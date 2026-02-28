# termesh-platform: 크로스플랫폼 윈도우 (winit + wgpu)

- phase: 1
- size: M
- blocked_by: 005-termesh-renderer

## 목표
- winit 기반 크로스플랫폼 윈도우 프레임워크
- 창 생성, 이벤트 루프, 렌더링 통합

## 완료 기준
- [x] winit EventLoop + ApplicationHandler 구현
- [x] 메인 윈도우 생성 (기본 1280x800, 최소 400x300)
- [x] wgpu Renderer 통합 (GPU 렌더링)
- [x] Terminal 통합 (그리드 렌더링)
- [x] 윈도우 리사이징 이벤트 처리
- [x] 키보드 입력 → 터미널 feed
- [x] SurfaceError 복구 (Lost, OutOfMemory)
- [x] clippy 경고 0개
- [x] `cargo test` 통과

## 포함할 모듈
- window: 윈도우 설정 및 생성
- event_loop: winit 이벤트 루프 통합

## 노트
- 원래 macOS 전용(Cocoa)이었으나 winit으로 크로스플랫폼 전환
- pollster로 async Renderer::new를 동기 호출
