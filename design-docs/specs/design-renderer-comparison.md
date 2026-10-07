# Canvas vs DOM Renderer Comparison

Status: Measured (rc-001; 24/24 runs complete, three runs per renderer/browser/size cell). Design: [DOM renderer design](design-dom-renderer.md).

<!-- RENDERER-COMPARISON:BEGIN -->
## Generated comparison

### 1000 lines · chromium

| Metric | Canvas median | DOM median | Winner | Threshold |
|---|---:|---:|---|---:|
| Input latency p50 | 7.200000047683716 ms | 8.299999952316284 ms | no clear difference | - |
| Input latency p95 | 16.399999928475154 ms | 16.099999999998545 ms | no clear difference | 50 ms |
| Input latency p99 | 16.800000023838948 ms | 17.09999997615887 ms | no clear difference | 100 ms |
| Frame interval p95 | 18.200000000004366 ms | 18.20000000001164 ms | no clear difference | 20 ms |
| Frame interval p99 | 18.600000000002183 ms | 18.599999999998545 ms | no clear difference | 50 ms |
| Text work p95 | 5.699999928474426 ms | 5.899999976158142 ms | no clear difference | 8 ms |
| Animation work p50 | 0.19999992847442627 ms | 0.20000004768371582 ms | no clear difference | 4 ms |
| Animation work p95 | 0.30000007152557373 ms | 0.3999999761581421 ms | no clear difference | - |
| Sync p95 | 20.5333333810122 ms | 20.581666595157003 ms | no clear difference | 33.4 ms |
| Sync p99 | 21.06366661899665 ms | 23.164666642813245 ms | no clear difference | 50 ms |
| Stall sync samples | 2 count | 1 count | no clear difference | - |
| Stall sync p50 | 53.663666666674544 ms | 89.33133338097832 ms | no clear difference | - |
| Stall sync p95 | 189.289999976143 ms | 89.33133338097832 ms | no clear difference | - |
| Stall sync max | 189.289999976143 ms | 89.33133338097832 ms | no clear difference | - |
| Late active mismatches | 0 count | 0 count | no clear difference | - |
| Replayed flashes | 0 count | 0 count | no clear difference | - |
| JS heap growth | 3869776 bytes | 3829108 bytes | no clear difference | - |
| DOM nodes peak | 843 count | 1258 count | canvas | - |
| DOM nodes final | 843 count | 1193 count | canvas | - |
| GPU ledger max | 7154396 bytes | 0 bytes | dom | - |
| Mount to first frame | 207.2 ms | 327 ms | no clear difference | - |
| Load to viewport | 30.69999999999999 ms | 70.10000000000002 ms | canvas | - |

#### Per-run values

| Renderer | Run | Values | Gate | Failures |
|---|---:|---|---|---|
| canvas | 1 | inputP50=7.399999952322105; inputP95=16.399999928475154; inputP99=16.699999928474426; frameP95=17.5; frameP99=17.60000000000582; textP95=6.100000023841858; animP50=0.19999992847442627; animP95=0.30000007152557373; syncP95=20.763666690516402; syncP99=21.06366661899665; stallSyncCount=2; stallSyncP50=53.663666666674544; stallSyncP95=220.5303333333286; stallSyncMax=220.5303333333286; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3866468; domNodesPeak=843; domNodesFinal=778; ledgerMaxBytes=7154396; mountToFirstFrameMs=138.4; loadToViewportMs=30.69999999999999 | pass | - |
| dom | 1 | inputP50=6.999999952327926; inputP95=15.899999952322105; inputP99=16.699999928474426; frameP95=18.10000000000582; frameP99=18.599999999976717; textP95=6.5; animP50=0.2999999523162842; animP95=0.3999999761581421; syncP95=17.718999976161285; syncP99=99.15233330946648; stallSyncCount=1; stallSyncP50=233.05233328565373; stallSyncP95=233.05233328565373; stallSyncMax=233.05233328565373; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3829108; domNodesPeak=1256; domNodesFinal=1191; ledgerMaxBytes=0; mountToFirstFrameMs=437.2; loadToViewportMs=72.69999992847443 | fail | syncAbsMs.p99=99.15233330946648 exceeds 50 |
| dom | 2 | inputP50=9.199999952310463; inputP95=16.600000047677895; inputP99=18.200000023840403; frameP95=18.20000000001164; frameP99=18.600000000002183; textP95=5.899999976158142; animP50=0.20000004768371582; animP95=0.5; syncP95=21.364666690511513; syncP99=23.164666642813245; stallSyncCount=1; stallSyncP50=89.33133338097832; stallSyncP95=89.33133338097832; stallSyncMax=89.33133338097832; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3764248; domNodesPeak=1258; domNodesFinal=1193; ledgerMaxBytes=0; mountToFirstFrameMs=327; loadToViewportMs=70.10000000000002 | pass | - |
| canvas | 2 | inputP50=7.100000023841858; inputP95=15.999999952313374; inputP99=16.800000023838948; frameP95=18.29999999998836; frameP99=18.60000000000582; textP95=5.699999928474426; animP50=0.19999992847442627; animP95=0.3999999761581421; syncP95=11.29000002383691; syncP99=21.02333338104654; stallSyncCount=3; stallSyncP50=105.75666671435465; stallSyncP95=189.289999976143; stallSyncMax=189.289999976143; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3869776; domNodesPeak=843; domNodesFinal=843; ledgerMaxBytes=7154396; mountToFirstFrameMs=329.3; loadToViewportMs=26.30000009536741 | pass | - |
| canvas | 3 | inputP50=7.200000047683716; inputP95=16.600000023841858; inputP99=18.40000002384477; frameP95=18.200000000004366; frameP99=18.600000000002183; textP95=4.100000023841858; animP50=0.10000002384185791; animP95=0.20000004768371582; syncP95=20.5333333810122; syncP99=53.36666673819127; stallSyncCount=0; stallSyncP50=unavailable; stallSyncP95=unavailable; stallSyncMax=unavailable; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3899464; domNodesPeak=881; domNodesFinal=881; ledgerMaxBytes=7154396; mountToFirstFrameMs=207.2; loadToViewportMs=37.600000071525585 | fail | syncAbsMs.p99=53.36666673819127 exceeds 50 |
| dom | 3 | inputP50=8.299999952316284; inputP95=16.099999999998545; inputP99=17.09999997615887; frameP95=18.20000000001164; frameP99=18.599999999998545; textP95=4.700000047683716; animP50=0.20000004768371582; animP95=0.30000007152557373; syncP95=20.581666595157003; syncP99=21.822999976160645; stallSyncCount=0; stallSyncP50=unavailable; stallSyncP95=unavailable; stallSyncMax=unavailable; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=3866712; domNodesPeak=1296; domNodesFinal=1296; ledgerMaxBytes=0; mountToFirstFrameMs=265; loadToViewportMs=37.90000009536743 | pass | - |

#### Gate results

Canvas: 2/3; DOM: 2/3.

### 1000 lines · webkit

| Metric | Canvas median | DOM median | Winner | Threshold |
|---|---:|---:|---|---:|
| Input latency p50 | 9 ms | 9 ms | no clear difference | - |
| Input latency p95 | 24 ms | 38 ms | no clear difference | 50 ms |
| Input latency p99 | 260 ms | 262 ms | no clear difference | 100 ms |
| Frame interval p95 | 19 ms | 19 ms | no clear difference | 20 ms |
| Frame interval p99 | 27 ms | 24 ms | no clear difference | 50 ms |
| Text work p95 | 4 ms | 4 ms | no clear difference | 8 ms |
| Animation work p50 | 0 ms | 0 ms | no clear difference | 4 ms |
| Animation work p95 | 1 ms | 1 ms | no clear difference | - |
| Sync p95 | 107.66666666667152 ms | 39.333333333343035 ms | no clear difference | 33.4 ms |
| Sync p99 | 191 ms | 70 ms | no clear difference | 50 ms |
| Stall sync samples | 2 count | 1 count | no clear difference | - |
| Stall sync p50 | 121.33333333334303 ms | 120.33333333334303 ms | no clear difference | - |
| Stall sync p95 | 237 ms | 204.66666666668607 ms | no clear difference | - |
| Stall sync max | 237 ms | 204.66666666668607 ms | no clear difference | - |
| Late active mismatches | 0 count | 0 count | no clear difference | - |
| Replayed flashes | 0 count | 0 count | no clear difference | - |
| JS heap growth | unavailable | unavailable | unavailable | - |
| DOM nodes peak | 862 count | 1275 count | canvas | - |
| DOM nodes final | 862 count | 1275 count | canvas | - |
| GPU ledger max | 7154396 bytes | 0 bytes | dom | - |
| Mount to first frame | 340 ms | 221 ms | no clear difference | - |
| Load to viewport | 112 ms | 81 ms | no clear difference | - |

#### Per-run values

| Renderer | Run | Values | Gate | Failures |
|---|---:|---|---|---|
| canvas | 1 | inputP50=9; inputP95=257; inputP99=266; frameP95=18; frameP99=19; textP95=4; animP50=0; animP95=1; syncP95=107.66666666667152; syncP99=110.66666666667152; stallSyncCount=1; stallSyncP50=335.33333333334303; stallSyncP95=335.33333333334303; stallSyncMax=335.33333333334303; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=797; ledgerMaxBytes=7154396; mountToFirstFrameMs=256; loadToViewportMs=51 | fail | inputLatencyMs.p95=257 exceeds 50, inputLatencyMs.p99=266 exceeds 100, syncAbsMs.p95=107.66666666667152 exceeds 33.4, syncAbsMs.p99=110.66666666667152 exceeds 50 |
| dom | 1 | inputP50=9; inputP95=258; inputP99=262; frameP95=19; frameP99=24; textP95=4; animP50=0; animP95=1; syncP95=26.333333333328483; syncP99=70; stallSyncCount=0; stallSyncP50=unavailable; stallSyncP95=unavailable; stallSyncMax=unavailable; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1275; domNodesFinal=1275; ledgerMaxBytes=0; mountToFirstFrameMs=221; loadToViewportMs=92 | fail | inputLatencyMs.p95=258 exceeds 50, inputLatencyMs.p99=262 exceeds 100, syncAbsMs.p99=70 exceeds 50 |
| dom | 2 | inputP50=9; inputP95=23; inputP99=260; frameP95=19; frameP99=27; textP95=4; animP50=0; animP95=1; syncP95=125.66666666667152; syncP99=134.9999999999709; stallSyncCount=4; stallSyncP50=120.33333333334303; stallSyncP95=204.66666666668607; stallSyncMax=204.66666666668607; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1275; domNodesFinal=1275; ledgerMaxBytes=0; mountToFirstFrameMs=290; loadToViewportMs=66 | fail | inputLatencyMs.p99=260 exceeds 100, syncAbsMs.p95=125.66666666667152 exceeds 33.4, syncAbsMs.p99=134.9999999999709 exceeds 50 |
| canvas | 2 | inputP50=11; inputP95=24; inputP99=31; frameP95=20; frameP99=32; textP95=7; animP50=0; animP95=1; syncP95=82; syncP99=191; stallSyncCount=2; stallSyncP50=38.33333333337214; stallSyncP95=128.33333333331393; stallSyncMax=128.33333333331393; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=862; ledgerMaxBytes=7154396; mountToFirstFrameMs=391; loadToViewportMs=154 | fail | syncAbsMs.p95=82 exceeds 33.4, syncAbsMs.p99=191 exceeds 50 |
| canvas | 3 | inputP50=9; inputP95=19; inputP99=260; frameP95=19; frameP99=27; textP95=4; animP50=0; animP95=1; syncP95=170.33333333334303; syncP99=387.33333333334303; stallSyncCount=6; stallSyncP50=121.33333333334303; stallSyncP95=237; stallSyncMax=237; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=862; ledgerMaxBytes=7154396; mountToFirstFrameMs=340; loadToViewportMs=112 | fail | inputLatencyMs.p99=260 exceeds 100, syncAbsMs.p95=170.33333333334303 exceeds 33.4, syncAbsMs.p99=387.33333333334303 exceeds 50 |
| dom | 3 | inputP50=9; inputP95=38; inputP99=264; frameP95=18; frameP99=18; textP95=3; animP50=0; animP95=1; syncP95=39.333333333343035; syncP99=41.33333333331393; stallSyncCount=1; stallSyncP50=238; stallSyncP95=238; stallSyncMax=238; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1275; domNodesFinal=1210; ledgerMaxBytes=0; mountToFirstFrameMs=197; loadToViewportMs=81 | fail | inputLatencyMs.p99=264 exceeds 100, syncAbsMs.p95=39.333333333343035 exceeds 33.4 |

#### Gate results

Canvas: 0/3; DOM: 0/3.

### 20000 lines · chromium

| Metric | Canvas median | DOM median | Winner | Threshold |
|---|---:|---:|---|---:|
| Input latency p50 | 8.100000047685171 ms | 7.8000000238389475 ms | no clear difference | - |
| Input latency p95 | 16.19999999999709 ms | 16.199999952310463 ms | no clear difference | 50 ms |
| Input latency p99 | 17.10000000000582 ms | 16.700000023840403 ms | no clear difference | 100 ms |
| Frame interval p95 | 17.5 ms | 17.5 ms | no clear difference | 20 ms |
| Frame interval p99 | 17.69999999999709 ms | 17.60000000000582 ms | no clear difference | 50 ms |
| Text work p95 | 4.700000047683716 ms | 5.200000047683716 ms | no clear difference | 8 ms |
| Animation work p50 | 0.19999992847442627 ms | 0.20000004768371582 ms | no clear difference | 4 ms |
| Animation work p95 | 0.30000007152557373 ms | 0.3999999761581421 ms | canvas | - |
| Sync p95 | 18.89400002383627 ms | 14.802666666666482 ms | no clear difference | 33.4 ms |
| Sync p99 | 51.494000000006054 ms | 29.480666666670004 ms | no clear difference | 50 ms |
| Stall sync samples | 2 count | 4 count | no clear difference | - |
| Stall sync p50 | 134.45666664282908 ms | 197.5359999761713 ms | no clear difference | - |
| Stall sync p95 | 213.15333333332092 ms | 248.00266673818987 ms | no clear difference | - |
| Stall sync max | 213.15333333332092 ms | 248.00266673818987 ms | no clear difference | - |
| Late active mismatches | 0 count | 0 count | no clear difference | - |
| Replayed flashes | 0 count | 0 count | no clear difference | - |
| JS heap growth | 5000224 bytes | 5076676 bytes | no clear difference | - |
| DOM nodes peak | 843 count | 1220 count | canvas | - |
| DOM nodes final | 843 count | 1220 count | canvas | - |
| GPU ledger max | 7154396 bytes | 0 bytes | dom | - |
| Mount to first frame | 208.7 ms | 209.8 ms | no clear difference | - |
| Load to viewport | 154.8000000238419 ms | 182.60000009536742 ms | no clear difference | - |

#### Per-run values

| Renderer | Run | Values | Gate | Failures |
|---|---:|---|---|---|
| canvas | 1 | inputP50=7.299999976152321; inputP95=16.599999928468606; inputP99=17.69999999999709; frameP95=17.799999999995634; frameP99=18.5; textP95=4.700000047683716; animP50=0.19999992847442627; animP95=0.2999999523162842; syncP95=14.586666595147108; syncP99=45.886666595120914; stallSyncCount=4; stallSyncP50=163.48666666666395; stallSyncP95=213.15333333332092; stallSyncMax=213.15333333332092; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=4862660; domNodesPeak=843; domNodesFinal=843; ledgerMaxBytes=7154396; mountToFirstFrameMs=209.1; loadToViewportMs=168.1999999761581 | pass | - |
| dom | 1 | inputP50=8.199999976161052; inputP95=16.299999999999272; inputP99=16.600000047677895; frameP95=17.5; frameP99=17.700000000004366; textP95=5.700000047683716; animP50=0.2999999523162842; animP95=0.3999999761581421; syncP95=14.802666666666482; syncP99=31.23600004769105; stallSyncCount=4; stallSyncP50=197.5359999761713; stallSyncP95=248.00266673818987; stallSyncMax=248.00266673818987; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=5076676; domNodesPeak=1220; domNodesFinal=1220; ledgerMaxBytes=0; mountToFirstFrameMs=209.8; loadToViewportMs=182.60000009536742 | pass | - |
| dom | 2 | inputP50=7.8000000238389475; inputP95=16.000000095365976; inputP99=16.90000007152412; frameP95=17.5; frameP99=17.60000000000582; textP95=5.200000047683716; animP50=0.20000004768371582; animP95=0.3999999761581421; syncP95=12.11400007152406; syncP99=29.480666666670004; stallSyncCount=5; stallSyncP50=223.18066671432462; stallSyncP95=255.88066671436536; stallSyncMax=255.88066671436536; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=5230904; domNodesPeak=1220; domNodesFinal=1220; ledgerMaxBytes=0; mountToFirstFrameMs=430.4; loadToViewportMs=203.79999999999995 | pass | - |
| canvas | 2 | inputP50=8.299999928480247; inputP95=16.19999999999709; inputP99=17.10000000000582; frameP95=17.5; frameP99=17.60000000000582; textP95=3.100000023841858; animP50=0.10000002384185791; animP95=0.30000007152557373; syncP95=19.190000023838365; syncP99=68.02333326183725; stallSyncCount=2; stallSyncP50=134.45666664282908; stallSyncP95=250.85666659512208; stallSyncMax=250.85666659512208; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=5074844; domNodesPeak=843; domNodesFinal=778; ledgerMaxBytes=7154396; mountToFirstFrameMs=208.7; loadToViewportMs=154.8000000238419 | fail | syncAbsMs.p99=68.02333326183725 exceeds 50 |
| canvas | 3 | inputP50=8.100000047685171; inputP95=15.900000071531394; inputP99=17.099999999998545; frameP95=17.5; frameP99=17.69999999999709; textP95=5.199999928474426; animP50=0.19999992847442627; animP95=0.30000007152557373; syncP95=18.89400002383627; syncP99=51.494000000006054; stallSyncCount=2; stallSyncP50=68.22733338104445; stallSyncP95=100.3606666905107; stallSyncMax=100.3606666905107; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=5000224; domNodesPeak=843; domNodesFinal=843; ledgerMaxBytes=7154396; mountToFirstFrameMs=127.8; loadToViewportMs=120.39999999999998 | fail | syncAbsMs.p99=51.494000000006054 exceeds 50 |
| dom | 3 | inputP50=7.799999952316284; inputP95=16.199999952310463; inputP99=16.700000023840403; frameP95=17.5; frameP99=17.60000000000582; textP95=4.5; animP50=0.20000004768371582; animP95=0.3999999761581421; syncP95=19.180666666667094; syncP99=19.514000047682202; stallSyncCount=4; stallSyncP50=51.71399997614208; stallSyncP95=234.5139999523526; stallSyncMax=234.5139999523526; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=4914232; domNodesPeak=1220; domNodesFinal=1155; ledgerMaxBytes=0; mountToFirstFrameMs=130.5; loadToViewportMs=112.39999997615814 | pass | - |

#### Gate results

Canvas: 1/3; DOM: 3/3.

### 20000 lines · webkit

| Metric | Canvas median | DOM median | Winner | Threshold |
|---|---:|---:|---|---:|
| Input latency p50 | 28 ms | 26 ms | no clear difference | - |
| Input latency p95 | 52 ms | 39 ms | no clear difference | 50 ms |
| Input latency p99 | 61 ms | 50 ms | no clear difference | 100 ms |
| Frame interval p95 | 18 ms | 18 ms | no clear difference | 20 ms |
| Frame interval p99 | 37 ms | 33 ms | no clear difference | 50 ms |
| Text work p95 | 4 ms | 3 ms | no clear difference | 8 ms |
| Animation work p50 | 0 ms | 0 ms | no clear difference | 4 ms |
| Animation work p95 | 1 ms | 1 ms | no clear difference | - |
| Sync p95 | 20 ms | 35 ms | no clear difference | 33.4 ms |
| Sync p99 | 56.333333333343035 ms | 52.33333333331393 ms | no clear difference | 50 ms |
| Stall sync samples | 3 count | 3 count | no clear difference | - |
| Stall sync p50 | 142.66666666668607 ms | 130.33333333334303 ms | no clear difference | - |
| Stall sync p95 | 248.66666666668607 ms | 197.66666666668607 ms | no clear difference | - |
| Stall sync max | 248.66666666668607 ms | 197.66666666668607 ms | no clear difference | - |
| Late active mismatches | 0 count | 0 count | no clear difference | - |
| Replayed flashes | 0 count | 0 count | no clear difference | - |
| JS heap growth | unavailable | unavailable | unavailable | - |
| DOM nodes peak | 862 count | 1239 count | canvas | - |
| DOM nodes final | 862 count | 1174 count | canvas | - |
| GPU ledger max | 7154396 bytes | 0 bytes | dom | - |
| Mount to first frame | 181 ms | 178 ms | no clear difference | - |
| Load to viewport | 327 ms | 519 ms | no clear difference | - |

#### Per-run values

| Renderer | Run | Values | Gate | Failures |
|---|---:|---|---|---|
| canvas | 1 | inputP50=24; inputP95=33; inputP99=43; frameP95=18; frameP99=31; textP95=3; animP50=0; animP95=1; syncP95=16.333333333343035; syncP99=31.666666666656965; stallSyncCount=3; stallSyncP50=218.66666666665697; stallSyncP95=248.66666666668607; stallSyncMax=248.66666666668607; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=862; ledgerMaxBytes=7154396; mountToFirstFrameMs=181; loadToViewportMs=306 | pass | - |
| dom | 1 | inputP50=26; inputP95=39; inputP99=50; frameP95=18; frameP99=33; textP95=3; animP50=0; animP95=1; syncP95=35; syncP99=52.33333333331393; stallSyncCount=3; stallSyncP50=148.66666666668607; stallSyncP95=185.33333333331393; stallSyncMax=185.33333333331393; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1239; domNodesFinal=1174; ledgerMaxBytes=0; mountToFirstFrameMs=178; loadToViewportMs=519 | fail | syncAbsMs.p95=35 exceeds 33.4, syncAbsMs.p99=52.33333333331393 exceeds 50 |
| dom | 2 | inputP50=29; inputP95=42; inputP99=58; frameP95=18; frameP99=33; textP95=3; animP50=0; animP95=1; syncP95=141.33333333332848; syncP99=175.66666666667152; stallSyncCount=2; stallSyncP50=107.33333333331393; stallSyncP95=249.33333333334303; stallSyncMax=249.33333333334303; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1239; domNodesFinal=1174; ledgerMaxBytes=0; mountToFirstFrameMs=439; loadToViewportMs=1012 | fail | syncAbsMs.p95=141.33333333332848 exceeds 33.4, syncAbsMs.p99=175.66666666667152 exceeds 50 |
| canvas | 2 | inputP50=33; inputP95=58; inputP99=61; frameP95=18; frameP99=41; textP95=4; animP50=0; animP95=1; syncP95=20; syncP99=56.333333333343035; stallSyncCount=5; stallSyncP50=142.66666666668607; stallSyncP95=267.66666666668607; stallSyncMax=267.66666666668607; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=797; ledgerMaxBytes=7154396; mountToFirstFrameMs=141; loadToViewportMs=327 | fail | inputLatencyMs.p95=58 exceeds 50, syncAbsMs.p99=56.333333333343035 exceeds 50 |
| canvas | 3 | inputP50=28; inputP95=52; inputP99=65; frameP95=18; frameP99=37; textP95=4; animP50=0; animP95=1; syncP95=22.666666666656965; syncP99=68.9999999999709; stallSyncCount=2; stallSyncP50=92; stallSyncP95=234; stallSyncMax=234; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=862; domNodesFinal=862; ledgerMaxBytes=7154396; mountToFirstFrameMs=214; loadToViewportMs=489 | fail | inputLatencyMs.p95=52 exceeds 50, syncAbsMs.p99=68.9999999999709 exceeds 50 |
| dom | 3 | inputP50=25; inputP95=36; inputP99=38; frameP95=18; frameP99=30; textP95=3; animP50=0; animP95=1; syncP95=27; syncP99=42.33333333332848; stallSyncCount=3; stallSyncP50=130.33333333334303; stallSyncP95=197.66666666668607; stallSyncMax=197.66666666668607; lateActiveMismatchCount=0; replayedFlashCount=0; heapGrowthBytes=unavailable; domNodesPeak=1239; domNodesFinal=1239; ledgerMaxBytes=0; mountToFirstFrameMs=137; loadToViewportMs=414 | pass | - |

#### Gate results

Canvas: 1/3; DOM: 1/3.

## Method and environment

Measurements reuse the shared workload, silent sink, browser launch settings and unchanged gates. Text work measures rAF JavaScript time; it excludes browser style, layout and paint. Canvas GPU execution is also excluded.

```json
{
  "runId": "rc-001",
  "hostname": "tacogipsnoMacBook-Air.local",
  "platform": "darwin",
  "release": "25.5.0",
  "arch": "arm64",
  "node": "v26.10.0",
  "cpus": 10,
  "lockOwner": "dom-renderer-evidence-s296",
  "wasm": {
    "path": "/Users/taco/gits/tacogips/vactr-worktrees/wf-dom-editor/editor/dist/vactr.wasm",
    "bytes": 6451822,
    "sha256": "fda9d3bac38b8f47b45d00d2dd890b844398e7899b10ee024aa48a213aaf870f",
    "nameSection": true,
    "dwarf": false,
    "profile": "release",
    "releasePath": "/Users/taco/gits/tacogips/vactr-worktrees/wf-dom-editor/target/wasm32-unknown-unknown/release/vactr.wasm",
    "releaseSha256": "fda9d3bac38b8f47b45d00d2dd890b844398e7899b10ee024aa48a213aaf870f",
    "debugSha256": "8c554118eeb1b23e405507c2814aaa2e6e4977c4888cfa77ae8fe5b4dd3c6a8d",
    "gating": true
  },
  "distBuild": "VACTR_REQUIRE_SESSION_ABI=1 npm run build",
  "command": "cd editor && node test/e2e/compare.mjs --run-id rc-001",
  "startedAt": "2026-10-07T06:42:39.686Z",
  "browserModes": {
    "webkit": {
      "headless": true,
      "probe": "headless WebGL2"
    }
  },
  "endedAt": "2026-10-07T07:09:37.160Z"
}
```

## Contended runs and attempts

Contended runs: 1000-chromium-dom-r1, 1000-chromium-dom-r2, 1000-webkit-canvas-r2, 1000-webkit-canvas-r3, 1000-webkit-dom-r2, 1000-webkit-dom-r3, 20000-chromium-dom-r1, 20000-chromium-dom-r2.

```json
[]
```

<!-- RENDERER-COMPARISON:END -->

## Analysis

The generated tables report the median of three runs per renderer in each browser and document size. Per-run values, gate outcomes, and the full raw metrics are in `design-docs/specs/evidence/renderer-comparison/rc-001/`.

- **End-to-end latency and renderer work.** In Chromium, 1,000-line input p95 was 16.4 ms for canvas and 16.1 ms for DOM; frame p95 was 18.2 ms for both, and text work p95 was 5.7 ms versus 5.9 ms. At 20,000 lines, input p95 was 16.2 ms for both, frame p95 17.5 ms for both, and text work p95 4.7 ms versus 5.2 ms. In WebKit, the corresponding 1,000-line values were 24 versus 38 ms input p95, 19 ms frame p95 for both, and 4 ms text work p95 for both. At 20,000 lines they were 52 versus 39 ms input p95, 18 ms frame p95 for both, and 4 versus 3 ms text work p95. Input and frame intervals include more of the user-visible path than the renderer's rAF work counters. `textWork` and `animationWork` exclude browser style, layout, and paint after the callback; canvas GPU execution is also excluded. The latency comparison is therefore the end-to-end view and the work counters are only the JavaScript portion.
- **Typing and geometry work.** Cumulative DOM `lineBuilds` per measured run had medians of 221 (Chromium, 1,000 lines), 360 (Chromium, 20,000), 222 (WebKit, 1,000), and 235 (WebKit, 20,000). Canvas `textBuilds` medians were 241, 311, 201, and 209 respectively. Normalizing each run by its `rawMetrics.editKeyCount`, median builds per editing key were DOM/canvas 0.381/0.414 (Chromium, 1,000), 0.636/0.549 (Chromium, 20,000), 0.388/0.348 (WebKit, 1,000), and 0.422/0.377 (WebKit, 20,000). Canvas median `geometryBytes` were 497,664, 524,288, 415,744, and 432,128 bytes in that same order; the renderer reports buffer uploads alongside those totals. The normalized counts include render builds from the full measured key sequence, not a single-character microbenchmark. The DOM window remains virtualized, so moving from 1,000 to 20,000 document lines does not make its live node count grow with the document.
- **Scroll and window movement.** DOM runs recorded three `windowShifts` per run. `transformWrites` were four in both Chromium cells and the WebKit 1,000-line cell, and eight in WebKit at 20,000 lines. DOM `lineReuses` medians were 10,084, 14,912, 10,295, and 11,218 for those cells. These counters show keyed reuse and transform updates in the DOM path. The canvas instrumentation does not expose directly comparable window-shift or transform-write counters, so no renderer winner is assigned for those mechanisms.
- **Animation path.** DOM recorded 536 `highlightToggles` and 204 `highlightBuilds` in each run; the toggles are class changes on the tracked highlight elements. Median animation-work p50/p95 was 0.2/0.4 ms for DOM versus 0.2/0.3 ms for canvas in Chromium at 1,000 lines; at 20,000 lines it was 0.2/0.4 versus 0.2/0.3 ms. WebKit values round to 0/1 ms for both renderers at both sizes. Canvas reports draw-call and geometry counters, but not a highlight-only count, so these measurements do not show a material animation-work win for either renderer.
- **Memory.** Chromium heap growth medians at 1,000 lines were 3,869,776 bytes for canvas and 3,829,108 for DOM; at 20,000 lines they were 5,000,224 and 5,076,676 bytes. Both remain below the 8 MiB threshold. DOM peak node medians were 1,258 and 1,220, versus 843 for canvas at either size. The DOM renderer's GPU ledger contribution was 0 bytes by construction; canvas reached 7,154,396 bytes in all Chromium and WebKit cells. WebKit JS heap growth is unavailable in this harness.
- **First viewport.** Median mount-to-first-frame times for canvas/DOM were 207.2/327 ms (Chromium, 1,000), 208.7/209.8 ms (Chromium, 20,000), 340/221 ms (WebKit, 1,000), and 181/178 ms (WebKit, 20,000). Median load-to-viewport was 30.7/70.1, 154.8/182.6, 112/81, and 327/519 ms respectively. The DOM backend had the shorter first-frame time in both WebKit cells; other cells were close or favored canvas.
- **Document-size sensitivity.** At 20,000 lines, Chromium's DOM input p95 stayed at 16.2 ms and frame p95 at 17.5 ms, matching the 1,000-line cell's 16.1/18.2 ms closely. DOM peak nodes were 1,220 at 20,000 lines versus 1,258 at 1,000, consistent with viewport virtualization. WebKit DOM input p95 was 39 ms at 20,000 versus 38 ms at 1,000; frame p95 was 18 versus 19 ms. The measured line-build and viewport-load values vary by run and browser as listed in the tables.
- **Browser and gate outcomes.** All three DOM Chromium 20,000-line runs passed their measurement gates; canvas passed one of three in that cell, with two sync-p99 failures. In Chromium at 1,000 lines, one of three runs failed for each renderer on sync p99. In WebKit at 1,000 lines all three runs failed for each renderer: canvas had input-latency and sync failures, while DOM had input-p99 and/or sync failures. At 20,000 WebKit lines, two of three runs failed for each renderer; canvas failures included input p95 and sync p99, while DOM failures were sync p95/p99. The unchanged thresholds are input p95 50 ms, input p99 100 ms, frame p95 20 ms, frame p99 50 ms, text p95 8 ms, sync p95 33.4 ms and sync p99 50 ms. These failures are recorded results; the matrix itself completed without incomplete run keys.
- **Run variance and contention.** The run records mark two of three DOM Chromium 1,000-line runs as contended, two of three runs for each WebKit renderer at 1,000 lines, and two of three DOM Chromium 20,000-line runs as contended. No 20,000-line WebKit runs were marked contended. The per-run table and raw `loadavgStart`/`loadavgEnd` fields preserve the variation; three-run medians should be read with those flags.

## Limitations

- WebKit does not provide JS heap growth in this measurement path; those cells report `unavailable`.
- WebKit behavior checks mark synthetic clipboard and composition as limitations and use synthetic pointer events for touch selection. They do not establish real WebKit system clipboard, physical IME, or touch behavior.
- WebKit ran headless, as recorded by the successful headless WebGL2 probe in `environment.json`.
- `loadToViewportMs` includes Playwright transfer of the full workload into the textarea as well as the first presented frame. It is not a renderer-only startup measure.
- Contended runs are retained in the matrix and are not discarded or repeated to select a better result. The exact runs and their contention flags are in each run JSON.
- All 24 measured runs completed. Gate failures described above remain part of the comparison and are shown per run in the generated tables.

## Observations for the canvas tuning proposal

These are measured observations only. In the 20,000-line Chromium cell, DOM input p95 was 16.2 ms versus 16.2 ms for canvas, while DOM sync p95 was 14.8 ms versus 18.9 ms; the DOM runs passed all three gates and two canvas runs exceeded sync p99. In 20,000-line WebKit, DOM input p95 was 39 ms versus canvas at 52 ms, and DOM text work p95 was 3 ms versus 4 ms; frame p95 was 18 ms for both. DOM sync p95 was 35 ms versus canvas at 20 ms, and both renderers had two gate-failing runs. DOM scrolling recorded three window shifts and four or eight transform writes per run, while corresponding canvas counters are unavailable. DOM highlights used 536 class toggles and 204 builds per run, with animation-work p95 at 0–1 ms across WebKit and 0.3–0.4 ms in Chromium; canvas animation work was 0.3 ms p95 in Chromium and 1 ms in WebKit. DOM used no GPU ledger bytes and had higher peak DOM node counts; the measurements do not report a DOM win in all metrics or browsers.
