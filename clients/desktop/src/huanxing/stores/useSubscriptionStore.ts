import { create } from 'zustand';
import * as subApi from '../lib/subscription-api';
import type {
  HxSubscriptionInfo,
  HxSubscriptionTier,
  HxCreditPackage,
  HxCreditHistory,
  HxPaymentResult,
} from '../lib/subscription-api';

interface SubscriptionState {
  subscription: HxSubscriptionInfo | null;
  tiers: HxSubscriptionTier[];
  packages: HxCreditPackage[];
  creditHistory: HxCreditHistory[];
  historyTotal: number;
  loading: boolean;

  fetchInfo: () => Promise<void>;
  fetchTiers: () => Promise<void>;
  fetchPackages: () => Promise<void>;
  fetchCreditHistory: () => Promise<void>;
  purchaseCredits: (packageId: number) => Promise<HxPaymentResult>;
}

export const useSubscriptionStore = create<SubscriptionState>((set) => ({
  subscription: null,
  tiers: [],
  packages: [],
  creditHistory: [],
  historyTotal: 0,
  loading: false,

  fetchInfo: async () => {
    set({ loading: true });
    try {
      const info = await subApi.getMyInfo();
      set({ subscription: info });
    } catch (err) {
      console.error('Failed to fetch subscription info', err);
    } finally {
      set({ loading: false });
    }
  },

  fetchTiers: async () => {
    try {
      const tiers = await subApi.getTiers();
      set({ tiers });
    } catch (err) {
      console.error('Failed to fetch tiers', err);
    }
  },

  fetchPackages: async () => {
    try {
      const packages = await subApi.getPackages();
      set({ packages });
    } catch (err) {
      console.error('Failed to fetch packages', err);
    }
  },

  fetchCreditHistory: async () => {
    try {
      const list = await subApi.getBalanceHistory();
      set({ creditHistory: list, historyTotal: list.length });
    } catch (err) {
      console.error('Failed to fetch credit history', err);
    }
  },

  purchaseCredits: async (packageId) => {
    return subApi.purchaseCredits(packageId);
  },
}));
