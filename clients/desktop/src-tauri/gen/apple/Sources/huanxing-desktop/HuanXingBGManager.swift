import Foundation
import BackgroundTasks
import UIKit
import UserNotifications

/// 唤星AI iOS 后台保活管理器
///
/// iOS 三层防御策略：
/// 1. **BGAppRefreshTask** — 系统周期性唤醒（~30s 执行窗口），用于 heartbeat & HASN ping
/// 2. **BGProcessingTask** — 长时间后台处理（充电 + WiFi 时），用于 agent 离线任务
/// 3. **Silent Push** — APNs 静默推送唤醒，用于紧急消息触发 agent 响应
///
/// 注意：iOS 不提供类似 Android Foreground Service 的永久后台能力。
/// 所有后台执行都受系统调度，频率取决于用户打开 app 的习惯。
@objc public class HuanXingBGManager: NSObject {
    
    @objc public static let shared = HuanXingBGManager()
    
    // BGTask identifiers（必须与 Info.plist 中注册的一致）
    private let refreshTaskId = "cn.dcfuture.huanxing.engine-refresh"
    private let maintenanceTaskId = "cn.dcfuture.huanxing.engine-maintenance"
    
    // 引擎端口
    private let enginePort: UInt16 = 42620
    
    private override init() {
        super.init()
    }
    
    // MARK: - 注册
    
    /// 在 application(_:didFinishLaunchingWithOptions:) 中调用
    @objc public func registerTasks() {
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: refreshTaskId,
            using: nil
        ) { [weak self] task in
            self?.handleRefreshTask(task as! BGAppRefreshTask)
        }
        
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: maintenanceTaskId,
            using: nil
        ) { [weak self] task in
            self?.handleMaintenanceTask(task as! BGProcessingTask)
        }
        
        NSLog("[HuanXing-BG] Background tasks registered")
    }
    
    // MARK: - 调度
    
    /// 调度下一次 BGAppRefreshTask（进入后台时调用）
    @objc public func scheduleRefresh() {
        let request = BGAppRefreshTaskRequest(identifier: refreshTaskId)
        // 最早 15 分钟后执行（系统会根据用户习惯优化）
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        
        do {
            try BGTaskScheduler.shared.submit(request)
            NSLog("[HuanXing-BG] Refresh task scheduled")
        } catch {
            NSLog("[HuanXing-BG] Failed to schedule refresh: \(error)")
        }
    }
    
    /// 调度 BGProcessingTask（用于耗时后台任务）
    @objc public func scheduleMaintenance() {
        let request = BGProcessingTaskRequest(identifier: maintenanceTaskId)
        request.requiresNetworkConnectivity = true
        request.requiresExternalPower = false // 不要求充电
        request.earliestBeginDate = Date(timeIntervalSinceNow: 60 * 60) // 1 小时后
        
        do {
            try BGTaskScheduler.shared.submit(request)
            NSLog("[HuanXing-BG] Maintenance task scheduled")
        } catch {
            NSLog("[HuanXing-BG] Failed to schedule maintenance: \(error)")
        }
    }
    
    // MARK: - 任务处理
    
    /// 短暂后台刷新（~30s 窗口）
    /// 执行：heartbeat ping + HASN 连接保活 + 重新调度下一次刷新
    private func handleRefreshTask(_ task: BGAppRefreshTask) {
        NSLog("[HuanXing-BG] Executing refresh task")
        
        // 设置过期处理
        task.expirationHandler = {
            NSLog("[HuanXing-BG] Refresh task expired")
            task.setTaskCompleted(success: false)
        }
        
        // 调度下一次刷新（链式调度）
        scheduleRefresh()
        
        // 执行 heartbeat ping
        performHealthPing { success in
            task.setTaskCompleted(success: success)
            NSLog("[HuanXing-BG] Refresh task completed: \(success)")
        }
    }
    
    /// 长时间后台处理
    /// 执行：agent 离线任务、memory 整理、HASN 消息同步
    private func handleMaintenanceTask(_ task: BGProcessingTask) {
        NSLog("[HuanXing-BG] Executing maintenance task")
        
        task.expirationHandler = {
            NSLog("[HuanXing-BG] Maintenance task expired")
            task.setTaskCompleted(success: false)
        }
        
        // 调度下一次维护
        scheduleMaintenance()
        
        // 对引擎执行维护操作
        performMaintenanceOperations { success in
            task.setTaskCompleted(success: success)
            NSLog("[HuanXing-BG] Maintenance task completed: \(success)")
        }
    }
    
    // MARK: - 引擎通信
    
    /// 发送 health ping 到引擎
    private func performHealthPing(completion: @escaping (Bool) -> Void) {
        guard let url = URL(string: "http://127.0.0.1:\(enginePort)/health") else {
            completion(false)
            return
        }
        
        var request = URLRequest(url: url)
        request.timeoutInterval = 10
        
        let task = URLSession.shared.dataTask(with: request) { data, response, error in
            if let error = error {
                NSLog("[HuanXing-BG] Health ping failed: \(error)")
                completion(false)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                NSLog("[HuanXing-BG] Health ping non-200")
                completion(false)
                return
            }
            
            NSLog("[HuanXing-BG] Health ping OK")
            completion(true)
        }
        task.resume()
    }
    
    /// 执行维护操作（触发引擎内部维护端点）
    private func performMaintenanceOperations(completion: @escaping (Bool) -> Void) {
        // 1. Health check
        performHealthPing { [weak self] healthy in
            guard healthy, let self = self else {
                completion(false)
                return
            }
            
            // 2. 触发 heartbeat tick（如果引擎有维护端点）
            guard let url = URL(string: "http://127.0.0.1:\(self.enginePort)/api/heartbeat/trigger") else {
                completion(true) // health 通过就算成功
                return
            }
            
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.timeoutInterval = 25
            
            let task = URLSession.shared.dataTask(with: request) { _, _, error in
                if let error = error {
                    NSLog("[HuanXing-BG] Maintenance heartbeat failed: \(error)")
                }
                completion(true)
            }
            task.resume()
        }
    }
    
    // MARK: - 推送处理
    
    /// 处理静默推送（APNs content-available）
    /// 从 application(_:didReceiveRemoteNotification:fetchCompletionHandler:) 调用
    @objc public func handleSilentPush(
        _ userInfo: [AnyHashable: Any],
        completion: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        NSLog("[HuanXing-BG] Received silent push")
        
        // 提取推送中的 action
        guard let action = userInfo["huanxing_action"] as? String else {
            // 默认行为：执行 health ping
            performHealthPing { success in
                completion(success ? .newData : .failed)
            }
            return
        }
        
        switch action {
        case "heartbeat":
            // 服务端要求立即执行一次 heartbeat
            performMaintenanceOperations { success in
                completion(success ? .newData : .failed)
            }
        case "hasn_sync":
            // 服务端要求同步 HASN 消息
            performHealthPing { success in
                completion(success ? .newData : .failed)
            }
        default:
            performHealthPing { success in
                completion(success ? .newData : .failed)
            }
        }
    }
}
