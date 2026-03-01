# UI 레이아웃: Focus 모드 (좌측 세션 리스트 + 우측 풀사이즈 터미널 + 사이드 패널)

- phase: 1
- size: M
- blocked_by: 007-termesh-layout

## 목표
- Codex 스타일 Focus 모드 UI 구현
- 세션 리스트, 터미널, 사이드 패널 3개 영역 통합

## 완료 기준
- [ ] 좌측 세션 리스트 패널 (200px 고정)
  - 세션 아이콘 (🤖 또는 🐚)
  - 세션명
  - 상태 아이콘 (⏳ 작업중 / ✅ 완료 / ✍️ 코드작성)
  - 선택 하이라이트
- [ ] 중앙 풀사이즈 터미널
  - 렌더러 통합
  - 실시간 출력 표시
- [ ] 우측 사이드 패널 (350px, 토글 가능)
  - Code Diff 탭
  - Preview 탭
  - Test Log 탭
  - Cmd+E로 토글
- [ ] 화면 리사이징 시 레이아웃 자동 조정
- [ ] 마우스 클릭으로 세션 선택
- [ ] 통합 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- session_list_panel: 세션 리스트 UI
- side_panel: 사이드 패널 관리
- layout_manager: Focus/Split 모드 토글

## 노트
- 터미널 크기: 중앙 영역에 맞게 동적 조정
- 사이드 패널 기본 탭: diff
