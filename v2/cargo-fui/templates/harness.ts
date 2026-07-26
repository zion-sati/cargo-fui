import { startManagedHarness, type HarnessExports } from '@effindomv2/runtime/managed-harness';

type AppExports = HarnessExports & {
  __runApp(): void;
  __disposeApp?(): void;
};

startManagedHarness({
  onReady: async (controller): Promise<void> => {
    await controller.loadApp({
      wasmPath: './app.wasm',
      run(exports: AppExports): void {
        exports.__runApp();
      },
      onDispose(exports: AppExports): void {
        exports.__disposeApp?.();
      },
    });
  },
});
