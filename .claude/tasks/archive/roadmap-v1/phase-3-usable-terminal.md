# Phase 3: Usable Terminal (완료)

완료일: 2026-03-01

## 태스크 요약

| # | 태스크 | 크기 | 요약 |
|---|--------|------|------|
| 029 | 커서 렌더링 | S | Renderer에 frame_count 기반 블링크 커서 (0.5s 주기) 구현 |
| 030 | 스크롤 지원 | S | AppCallbacks에 on_scroll 추가, MouseWheel 이벤트 처리 |
| 031 | 선택 & 클립보드 | M | 마우스 드래그 선택, Ctrl+Shift+C/V 복사/붙여넣기, arboard 크레이트 |
| 032 | Auth Gate 연결 | S | 앱 시작 시 check_auth_local() 호출, trial mode 폴백 |

## 주요 변경

- `termesh-terminal`: Selection 모델 (anchor/end point, normalized range, text extraction)
- `termesh-renderer`: 커서 블링크 렌더링, 선택 영역 하이라이트 (blue bg)
- `termesh-platform`: 마우스 이벤트 (CursorMoved, MouseInput), 클립보드 (arboard), on_scroll
- `termesh-input`: Copy/Paste 액션, Ctrl+Shift+C/V 바인딩
- `termesh-app`: Auth gate, env_logger 초기화

## 커밋

- `70ef5cc` feat: add cursor rendering, scroll support, and auth gate wiring
- `26a4f8e` feat: add mouse text selection and clipboard support
