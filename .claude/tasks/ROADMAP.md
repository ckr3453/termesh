# Termesh 로드맵

## Phase 1: 엔진 구현 ✅

**목표**: 각 크레이트의 핵심 로직을 독립적으로 구현하고 테스트

- [x] 001~009e: 터미널 기초 (PTY, 터미널 에뮬, 렌더러, 이벤트 루프)
- [x] 010~017: 에이전트/diff/레이아웃/프리셋 엔진 구현

---

## Phase 2: 크로스 플랫폼 + 어댑터 확장 ✅

**목표**: Windows/Linux 지원, 멀티 에이전트, 인증

- [x] 018~020: 크로스 플랫폼 (ConPTY, wgpu 백엔드, CI/CD)
- [x] 021~023: Gemini/Codex 어댑터
- [x] 024~028: 인증, DX 개선

---

## Phase 3: 실사용 가능한 터미널 ✅

**목표**: "cargo run하면 일반 터미널처럼 쓸 수 있다"

### 기본 터미널 완성 (4 tasks)
- [x] 커서 렌더링 (블록/라인 커서 + 깜빡임)
- [x] 스크롤 (마우스 휠 + 스크롤백 버퍼)
- [x] 선택 & 복사/붙여넣기 (마우스 드래그, Ctrl+Shift+C/V)
- [x] 인증 게이트 연결 (시작 시 check_auth_local 호출)

---

## Phase 4: 멀티세션 + 화면 분할 ✅

**목표**: "Win+T로 새 세션, Win+H/J/K/L로 이동, 화면 분할"

### 액션 핸들러 연결 (4 tasks)
- [x] Action → SessionManager 연결 (spawn/close/focus)
- [x] on_tick() 멀티그리드 반환 (SplitLayout → 좌표 계산)
- [x] 패인 경계선 렌더링 (구분선 + 활성 패인 하이라이트)
- [x] 세션별 독립 PTY resize

---

## Phase 5: AI 에이전트 관제탑 ✅

**목표**: "에이전트 상태가 보이고, diff가 실시간으로 뜬다"

### 에이전트 통합 (2 tasks)
- [x] PTY 출력 → AdapterRegistry → 에이전트 상태 업데이트 파이프라인
- [x] 워크스페이스 프리셋 → 실제 세션 스폰 연결

### UI 패널 렌더링 (3 tasks)
- [x] 세션 리스트 패널 렌더링 (좌측, 에이전트 상태 아이콘)
- [x] 사이드 패널 렌더링 (우측, diff/preview 탭)
- [x] 파일 워처 기동 + diff 결과 사이드 패널 표시

---

## Phase 5.5: 코드 품질 리팩토링 ✅

**목표**: 보안 위험, 메모리 비효율, 코드 품질 이슈를 정리하여 안정성과 유지보수성 확보

### 메모리/효율 (2 tasks)
- [x] 029: 렌더 루프 메모리 최적화 (매 프레임 Vec 할당 → 재사용 버퍼)
- [x] 030: diff history 더블 클론 제거 (cache.insert() 반환값 활용)

### 에러 처리/안전성 (1 task)
- [x] 031: PTY 스레드 panic → Result 전환

### 코드 정리 (2 tasks)
- [x] 033: #[allow(dead_code)] 정리 + 스텁 탭(Preview/TestLog) 제거
- [x] 034: 에러 메시지 경로 노출 제거 + silent 에러 로깅

---

## Phase 5.6: 사이드 패널 개선 ✅

**목표**: diff 뷰를 실용적 수준으로 강화, 렌더링 품질 향상

### Diff UX (2 tasks)
- [x] 035: 변경 파일 목록 + 파일 선택 diff (blocked_by: 033)
- [x] 036: Unified / Side-by-side diff 모드 전환 (blocked_by: 035)

### 렌더링 품질 (1 task)
- [x] 037: MSDF 폰트 렌더링 도입 (blocked_by: 029)

---

## Phase 6: 렌더러 업그레이드 (renderer-upgrade) ✅

**목표**: 네이티브 터미널 수준의 렌더링 성능과 품질 확보

### 렌더 루프 + GPU 최적화 (4 tasks)
- [x] Dirty-based 렌더 루프 (idle CPU 절감, 500ms 커서 블링크 타이머)
- [x] GPU 버퍼 재사용 (매 프레임 create_buffer_init 제거, amortized O(1) 성장)
- [x] wgpu 28 업그레이드 + glyphon/cosmic-text 전환
- [x] 시스템 단축키 확장 (Cmd+A/T/W/Q/F, 하드코딩 → keymap 일반화)

---

## Phase 7: 렌더링 품질 + 호환성 ✅

**목표**: "이모지 안 깨지고, 한글 잘 나오고, 입력 안 밀리고, 스크롤 안 버벅이는" 수준

### 렌더링 품질 (4 tasks)
- [x] 이모지 너비 정정 (unicode-width 통합, wide:bool → width:u8, Apple Color Emoji fallback)
- [x] CJK Fallback 폰트 등록 + 폰트 높이 수정 (cosmic-text monospace family, 메트릭 개선)
- [x] IME Preedit UI (한글 조합 미리보기, 반투명 배경 + 밑줄, set_ime_cursor_area 동기화)
- [x] 스크롤 성능 최적화 (dirty rows 행별 추적, glyphon Buffer 재사용, shaping 스킵)

---

## Phase 8: 기본 터미널 완성도

**목표**: iTerm2 대체 가능한 수준의 일상 터미널

### Action 핸들러 구현 (2 tasks)
- [ ] Quit/CloseTab/NewTab 핸들러 연결 (기존 SessionManager 메서드 연결)
- [ ] SelectAll/Find 구현 (선택 영역 로직 + 검색 UI 패널)

### 코드 정리 (2 tasks)
- [ ] 4개 에이전트 동시 수정 통합 검증 (renderer.rs 충돌 확인, cargo build/test)
- [ ] #[allow(dead_code)] 7곳 정리 + agent_picker 테스트 실패 3건 수정

### 색공간 + 렌더링 고도화 (2 tasks)
- [ ] sRGB 색공간 정리 (renderer.rs:144 의도 명확화 또는 sRGB 전환)
- [ ] 서브픽셀 렌더링 + Color emoji COLR 지원 (glyphon 의존)

---

## Phase 9: 사용자 설정 시스템

**목표**: 키바인딩/테마를 사용자가 커스터마이징 가능

- [ ] config/default.toml에서 keymap 로드 (현재 Serialize/Deserialize 준비됨)
- [ ] 사용자 정의 색상 스킴 / 테마 설정

---

## Phase 10: 원격 접속

**목표**: PC에서 나가도 폰으로 이어서 작업

- (설계 미정)

---
