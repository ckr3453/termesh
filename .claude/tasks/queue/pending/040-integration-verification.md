# 040: Phase 6-7 통합 검증 + 코드 정리

**Phase**: 8 - 기본 터미널 완성도
**Priority**: 높음
**Difficulty**: 낮음

## 목표
renderer-upgrade (Phase 6) + 품질 개선 (Phase 7)에서 4개 에이전트가 동시에 수정한 코드의 통합 검증.

## 작업 내용

### 통합 검증
1. `cargo build --all` 통과 확인
2. `cargo test --all` 통과 확인
3. renderer.rs에서 emoji-agent, ime-agent, scroll-agent 변경 간 충돌/불일치 확인
4. `render_grids()` 시그니처 일관성 확인 (preedit 파라미터 추가됨)
5. `cargo run --bin termesh-app` 실행하여 체감 테스트:
   - 이모지 (echo "🎉👨‍👩‍👧‍👦")
   - 한글 입력 (IME 조합 미리보기)
   - 대량 출력 스크롤 (find / -name "*.rs")
   - CJK 혼합 텍스트

### 코드 정리
1. `#[allow(dead_code)]` 7곳 확인 후 불필요 시 제거
2. agent_picker 테스트 실패 3건 수정 (pre-existing)
3. 미사용 import/변수 정리

## 변경 파일
- 여러 크레이트 (검증 결과에 따라)

## 검증
- `cargo build --all` 경고 0
- `cargo test --all` 전체 통과
- `cargo clippy --all` 경고 0
