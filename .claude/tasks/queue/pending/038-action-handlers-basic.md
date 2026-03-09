# 038: Quit/CloseTab/NewTab Action 핸들러 연결

**Phase**: 8 - 기본 터미널 완성도
**Priority**: 높음
**Difficulty**: 낮음

## 목표
미구현 Action 핸들러 3개를 기존 메서드에 연결.

## 배경
main.rs:1049-1051에서 SelectAll/NewTab/CloseTab/Quit/Find가 `log::info!("action not yet implemented")` 상태.
이 중 Quit/CloseTab/NewTab은 기존 코드와 연결만 하면 됨.

## 작업 내용
1. `Action::Quit` -> `event_loop.exit()` 또는 `elwt.exit()` 호출
2. `Action::CloseTab` -> `session_mgr.close_session(active_session_id)` 연결
3. `Action::NewTab` -> `session_mgr.spawn_session(default_config)` 연결
4. 각 핸들러에서 dirty = true 설정

## 변경 파일
- `crates/termesh-app/src/main.rs`

## 검증
- Cmd+Q로 앱 종료
- Cmd+T로 새 세션 생성
- Cmd+W로 현재 세션 닫기
- `cargo test --all` 통과
