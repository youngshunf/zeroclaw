import UIKit
import UserNotifications

/// 唤星AI iOS AppDelegate
///
/// 负责：
/// 1. 注册 BGTask 后台任务
/// 2. 注册 APNs 远程推送
/// 3. 处理静默推送唤醒
/// 4. 管理前后台切换时的 BGTask 调度
@objc public class HuanXingAppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    
    public func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        // 注册 BGTask
        HuanXingBGManager.shared.registerTasks()
        
        // 注册推送通知
        registerForPushNotifications(application)
        
        NSLog("[HuanXing] App launched")
        return true
    }
    
    // MARK: - 前后台切换
    
    public func applicationDidEnterBackground(_ application: UIApplication) {
        NSLog("[HuanXing] Entering background — scheduling tasks")
        // 进入后台时调度 BGTask
        HuanXingBGManager.shared.scheduleRefresh()
        HuanXingBGManager.shared.scheduleMaintenance()
    }
    
    public func applicationWillEnterForeground(_ application: UIApplication) {
        NSLog("[HuanXing] Entering foreground")
        // 前台不需要 BGTask，取消已调度的
        // （系统会在 app 前台时自动暂停 BGTask）
    }
    
    // MARK: - 推送注册
    
    private func registerForPushNotifications(_ application: UIApplication) {
        let center = UNUserNotificationCenter.current()
        center.delegate = self
        
        center.requestAuthorization(options: [.alert, .badge, .sound]) { granted, error in
            if let error = error {
                NSLog("[HuanXing] Push auth error: \(error)")
                return
            }
            
            if granted {
                DispatchQueue.main.async {
                    application.registerForRemoteNotifications()
                }
                NSLog("[HuanXing] Push notification permission granted")
            } else {
                NSLog("[HuanXing] Push notification permission denied")
            }
        }
    }
    
    // MARK: - APNs 回调
    
    public func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let token = deviceToken.map { String(format: "%02.2hhx", $0) }.joined()
        NSLog("[HuanXing] APNs device token: \(token)")
        
        // TODO: 将 token 发送到唤星云端，用于静默推送唤醒
        // 通过引擎的 API 上传: POST /api/push/register { "platform": "ios", "token": "..." }
    }
    
    public func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        NSLog("[HuanXing] APNs registration failed: \(error)")
    }
    
    // MARK: - 静默推送处理
    
    public func application(
        _ application: UIApplication,
        didReceiveRemoteNotification userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        HuanXingBGManager.shared.handleSilentPush(userInfo, completion: completionHandler)
    }
    
    // MARK: - 前台推送显示
    
    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        // 前台也显示通知（用于 HASN 消息提醒）
        return [.banner, .badge, .sound]
    }
    
    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let userInfo = response.notification.request.content.userInfo
        NSLog("[HuanXing] Notification tapped: \(userInfo)")
        // TODO: 根据 userInfo 跳转到对应页面（聊天/HASN/通知）
    }
}
