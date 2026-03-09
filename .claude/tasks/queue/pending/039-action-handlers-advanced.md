# 039: SelectAll/Find Action 구현

**Phase**: 8 - 기본 터미널 완성도
**Priority**: 중간
**Difficulty**: 중~높음
**Blocked by**: 038

## 목표
SelectAll과 Find 기능 구현.

## 작업 내용

### SelectAll
1. 현재 활성 터미널의 전체 스크롤백 + 화면 내용을 선택 영역으로 설정
2. 선택 영역 렌더링 (배경색 반전)
3. 선택 상태에서 Cmd+C로 복사 가능

### Find (검색 UI)
1. Cmd+F 시 상단에 검색 바 오버레이 표시
2. 입력 필드 + 이전/다음 버튼
3. 스크롤백 버퍼 포함 전체 텍스트 검색
4. 매치 하이라이트 + 현재 매치로 스크롤
5. Escape로 검색 바 닫기

## 변경 파일
- `crates/termesh-app/src/main.rs` (핸들러)
- `crates/termesh-renderer/src/renderer.rs` (검색 바 렌더링)
- `crates/termesh-terminal/src/terminal.rs` (텍스트 검색)
- `crates/termesh-platform/src/event_loop.rs` (검색 모드 입력 처리)

## 검증
- Cmd+A로 전체 선택 후 Cmd+C로 복사
- Cmd+F로 검색 바 표시, 텍스트 검색 + 하이라이트
- `cargo test --all` 통과
