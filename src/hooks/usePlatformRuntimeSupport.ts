import { useMemo } from 'react';

export type PlatformRuntimeSupport = 'desktop' | 'macos-only' | 'macos-windows';

export function usePlatformRuntimeSupport(mode: PlatformRuntimeSupport): boolean {
  return useMemo(() => {
    if (typeof navigator === 'undefined') return false;
    const platform = navigator.platform || '';
    const ua = navigator.userAgent || '';
    const isMac = /mac/i.test(platform) || /mac/i.test(ua);
    const isWindows = /win/i.test(platform) || /windows/i.test(ua);
    if (mode === 'macos-only') {
      return isMac;
    }
    if (mode === 'macos-windows') {
      return isMac || isWindows;
    }
    const isLinux = /linux/i.test(platform) || /linux/i.test(ua);
    return isMac || isWindows || isLinux;
  }, [mode]);
}
