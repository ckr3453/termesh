# termesh-platform: macOS 네이티브 윈도우 (NSApplication/NSView)

- phase: 1
- size: M
- blocked_by: 005-termesh-renderer

## 목표
- macOS 네이티브 윈도우 프레임워크 (Cocoa)
- 창 생성, 이벤트 루프, 렌더링 통합

## 완료 기준
- [ ] `NSApplication` 초기화 및 디스패치
- [ ] 메인 윈도우 (`NSWindow`) 생성
- [ ] 커스텀 `NSView` 서브클래싱 (렌더링 대상)
- [ ] 렌더링 루프 (Metal/OpenGL 브리지)
- [ ] 윈도우 리사이징 이벤트 처리
- [ ] 단위 테스트 (unsafe 블록 검증)
- [ ] `cargo test` 통과

## 포함할 모듈
- window: NSWindow 래퍼
- view: 커스텀 NSView 구현
- event_loop: macOS 이벤트 루프 통합

## 노트
- unsafe Cocoa 바인딩: `objc2` 또는 `cocoa` 크레이트
- 스레드 안전성: ui_thread에서만 NSView 조작
