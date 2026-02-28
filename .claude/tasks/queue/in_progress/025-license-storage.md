# 로컬 라이선스 저장소

- phase: 2
- size: S
- blocked_by: 024-auth-api-client

## 목표
- 인증 토큰을 안전하게 로컬에 저장/조회/삭제

## 완료 기준
- [ ] 플랫폼별 키체인/credential store 연동 (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- [ ] 토큰 저장/조회/삭제 API
- [ ] 키체인 미지원 시 암호화된 파일 폴백
- [ ] 유닛 테스트 통과
