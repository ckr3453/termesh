# 에이전트 상태 표시 (세션 리스트 아이콘/스피너 + 사이드 패널 상세)

- phase: 1
- size: S
- blocked_by: 010-termesh-agent, 013-focus-mode-ui

## 목표
- 에이전트 상태를 시각적으로 표시
- Focus 모드: 사이드 패널에 상세 정보
- Split 모드: pane 상단 바에 아이콘만

## 완료 기준
- [ ] 상태 아이콘 정의:
  - ⏳ Thinking (생각 중)
  - ✍️ WritingCode (코드 작성)
  - ▶️ RunningCommand (커맨드 실행)
  - ⏸️ WaitingForInput (입력 대기)
  - ✅ Success (성공)
  - ✗ Error (에러)
  - 💤 Idle (대기 중)
- [ ] Focus 모드 사이드 패널: 에이전트 상세 정보
  - 현재 상태 + 타임스탬프
  - 마지막 변경 파일
  - 마지막 커맨드 (성공/실패)
- [ ] Split 모드: 각 pane 상단 바에 상태 아이콘 + 스피너
  - Thinking/WritingCode/Running 시 회전 애니메이션
- [ ] 상태 업데이트 빈도: 100ms
- [ ] 통합 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- status_display: 상태 표시 로직
- status_panel: Focus 모드 사이드 패널

## 노트
- 스피너 프레임: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
