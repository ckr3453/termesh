# termesh-renderer: wgpu GPU 렌더링 파이프라인

- phase: 1
- size: L
- blocked_by: 004-termesh-terminal

## 목표
- wgpu 기반 GPU 렌더링 파이프라인 구축
- Monospace 폰트의 글리프 래스터라이징
- 셀 기반 터미널 그리드를 화면에 렌더링

## 완료 기준
- [ ] `Renderer` 구조체: wgpu device/queue/surface 관리
- [ ] 폰트 로드 및 글리프 캐시 (freetype-rs 또는 fontdue)
- [ ] 렌더 파이프라인: 쉐이더 작성 (WGSL 또는 GLSL)
- [ ] 프레임 렌더링: 셀 → 텍스트 + 배경 색상 매핑
- [ ] FPS 제한 (60fps)
- [ ] 텍스처 아틀라스로 글리프 캐싱
- [ ] 단위 테스트 + 렌더링 결과 시각 검증
- [ ] `cargo test` 통과

## 포함할 모듈
- renderer: 렌더러 메인 구조체
- pipeline: wgpu 렌더 파이프라인
- font: 폰트 로드, 글리프 래스터라이징
- glyph_cache: 글리프 캐싱 전략
- shader: WGSL 쉐이더 코드

## 노트
- 참고: Tide의 wgpu 렌더링 파이프라인
- 초기 지원 폰트: SF Mono (macOS), Menlo
- 다크 테마: 배경 #1e1e1e, 텍스트 #e0e0e0
