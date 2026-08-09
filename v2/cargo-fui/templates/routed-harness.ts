import { startRoutedHarness, type RoutedHarnessRoute } from '@effindomv2/runtime/routed-harness';
import type { HarnessExports } from '@effindomv2/runtime/managed-harness';
import routeManifest from './routes.json' with { type: 'json' };

type RouteExports = HarnessExports & { __runApp(): void; __disposeApp?(): void };
const routes: readonly RoutedHarnessRoute[] = routeManifest.routes;

startRoutedHarness<RouteExports>({
  shellId: 'fui-routes',
  routeBase: '/',
  routes,
  recreateRuntimeOnWarmRouteSwap: true,
  run(exports): void { exports.__runApp(); },
  onDispose(exports): void { exports.__disposeApp?.(); },
});
