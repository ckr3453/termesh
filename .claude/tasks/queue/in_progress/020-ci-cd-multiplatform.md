# CI/CD 멀티 플랫폼 빌드 파이프라인

- phase: 2
- size: M
- blocked_by: 019-wgpu-backend-compat

## 목표
- GitHub Actions로 macOS/Windows/Linux 자동 빌드 및 테스트

## 완료 기준
- [ ] GitHub Actions 워크플로우 파일 작성
- [ ] macOS (x86_64 + aarch64) 빌드 + 테스트
- [ ] Windows (x86_64) 빌드 + 테스트
- [ ] Linux (x86_64) 빌드 + 테스트
- [ ] 릴리즈 시 플랫폼별 바이너리 자동 생성
- [ ] 빌드 캐싱으로 CI 시간 최적화
