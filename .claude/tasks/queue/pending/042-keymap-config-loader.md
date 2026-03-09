# 042: 키바인딩 설정 파일 로드

**Phase**: 9 - 사용자 설정 시스템
**Priority**: 중간
**Difficulty**: 중간

## 목표
config/default.toml에서 사용자 정의 키바인딩을 로드하여 default_keymap()을 오버라이드.

## 배경
현재 Keymap, Keybinding, Action 모두 Serialize/Deserialize가 구현되어 있고,
parse_binding("Cmd+T") 파서도 있음. 설정 파일 로더만 추가하면 됨.

## 작업 내용
1. config/default.toml에 `[keybindings]` 섹션 정의
   ```toml
   [keybindings]
   "Cmd+T" = "NewTab"
   "Cmd+W" = "CloseTab"
   # 사용자가 추가/변경 가능
   ```
2. 앱 시작 시 설정 파일 로드 → default_keymap() 위에 오버라이드
3. 설정 파일 없으면 default_keymap() 그대로 사용
4. 잘못된 바인딩은 경고 로그 후 무시

## 변경 파일
- `crates/termesh-core/src/config.rs` (또는 신규)
- `crates/termesh-app/src/main.rs` (로드 + 적용)
- `config/default.toml` (keybindings 섹션 추가)

## 검증
- 설정 파일에서 키바인딩 변경 후 앱에 반영 확인
- 잘못된 바인딩 시 앱 크래시 없이 경고 로그
- `cargo test --all` 통과
