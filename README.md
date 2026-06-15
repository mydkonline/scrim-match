# Scrim.GG — 비밀 보장 스크림 매칭

리그 오브 레전드 · VALORANT · StarCraft 프로/세미프로 팀을 위한 **비공개 스크림(연습경기) 매칭 플랫폼**.
시리얼 코드로 비공개 입장 → 종목·슬롯 선택 → **클릭 한 번으로 전 세계 팀과 자동 페어링** → 두 팀만 아는 비밀 코드로 매칭 확정.

> 디자인 기반: `npx getdesign add supabase` (Supabase 디자인 언어 — 화이트 캔버스 · 단일 이메랄드 CTA · Inter).

## 스택

| 레이어 | 기술 |
|---|---|
| 프론트엔드 | **Rust + Dioxus** (WASM), Trunk 빌드 |
| 백엔드 | **Rust + Axum** REST + WebSocket 실시간 매칭 |
| 공용 타입 | `shared` 크레이트 (프론트·백 공유) |
| 배포 | 프론트 → GitHub Pages · 백엔드 → Fly.io/Railway/Shuttle |

## 4개 화면

1. **Login** — OP.GG 브랜딩, 시리얼 코드, 종목 선택, 우리 팀 선택
2. **Matching** — 우리 팀 ↔ 상대 팀 로스터, 슬롯(날짜·시간) 지정, Find Scrim → 비밀 코드 카드 → Apply / Denied → 확정(Yes)
3. **Team Setting** — Manager/Coach, 1군·2군·Academy 로스터
4. **Calendar** — 2026 월별 스크림 일정 (VS 상대, 결과)

## 구조

```
scrim-match/
├── shared/     공용 도메인 타입 + WS 프로토콜 + 시드 팀 데이터
├── backend/    Axum 서버 (REST + /ws 실시간 매칭) + Dockerfile
├── frontend/   Dioxus WASM 앱 (4화면)
└── .github/workflows/deploy.yml   GitHub Pages 자동 배포
```

## 로컬 개발

> ⚠️ 이 머신은 PATH 의 `cargo` 가 Homebrew 라 wasm std 가 없습니다.
> 프론트 빌드 시 rustup 툴체인을 앞에 둬야 합니다:
> `export PATH="$(rustc --print sysroot 2>/dev/null)/bin:$PATH"` 또는 아래처럼 rustup 경로 사용.

```bash
# 백엔드 (REST + WS, http://localhost:8080)
cargo run -p backend

# 프론트엔드 (http://localhost:8080 백엔드에 연결하려면 BACKEND_URL 지정)
cd frontend
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
BACKEND_URL=http://localhost:8080 trunk serve --open
```

`BACKEND_URL` 미지정 시 프론트는 내장 시드 데이터로 동작하는 **오프라인 데모 모드**로 빌드됩니다.

## 배포

### 프론트엔드 (GitHub Pages) — 자동
`main` 브랜치 push 시 `.github/workflows/deploy.yml` 가 빌드 후 Pages 에 배포합니다.
- 저장소 **Settings → Pages → Source: GitHub Actions** 로 설정.
- 실서버 연결 시: **Settings → Secrets and variables → Actions → Variables** 에 `BACKEND_URL` 추가 (예: `https://scrim-gg.fly.dev`).

### 백엔드 (실시간 서버) — 별도 호스트 필요
GitHub Pages 는 정적 호스팅이라 WebSocket 서버를 띄울 수 없습니다. 아래 중 하나로 배포:

```bash
# Fly.io (예시)
fly launch --no-deploy   # fly.toml 사용
fly deploy
```

배포 후 발급된 URL 을 위의 `BACKEND_URL` 변수에 넣으면 프론트가 실서버에 연결됩니다.

## 비밀 보장 매칭 흐름 (WebSocket)

```
Client ──Hello{serial, team, game}──▶ Server   (시리얼 코드 인증)
Client ──FindScrim{date, time}─────▶ Server   (슬롯 대기열 진입)
        ◀── MatchOffer{scrim, code} ──        (같은 슬롯 상대와 페어링, 비밀 코드 발급)
Client ──Apply{match_id}───────────▶ Server
        ◀── MatchUpdate{Confirmed} ──         (양쪽 수락 → 확정, 두 팀만 코드 공유)
```

## 터미널 CLI (브라우저 없이 신청 수신/수락)

브라우저를 켜지 않아도 터미널에서 스크림 신청을 받고 수락할 수 있습니다. 접속 시 메시지 큐에 쌓인 신청이 전달됩니다.

```bash
# 우리 팀으로 접속(시리얼은 팀ID로 자동 계산, 다른 서버는 --url 로)
cargo run -p cli -- --team bnk-lol
#   옵션: --game lol|val|sc  --serial <코드>  --url wss://scrim-gg.fly.dev/ws

# 접속 후 명령
#   list                 받은 신청 목록(코드)
#   accept <code>        코드로 신청 수락 → 매칭 확정
#   apply <teamId>       상대 팀에 스크림 신청(코드 발급)
#   chat <id> <메시지>   확정 매칭에 채팅
#   quit                 종료
```

예: 다른 팀이 `apply bnk-lol` 로 신청 → 그 팀은 코드를 받고, BNK는 (나중에 켜도) `list` → `accept <code>` 로 확정.

## TODO (배포 후 개선)
- [ ] 시리얼 코드 → 팀 매핑 및 실제 인증/권한
- [ ] DB(Postgres) 연동, 인메모리 상태 대체
- [ ] 로스터 1군/2군/Academy 스왑 영속화
- [ ] 매칭 필터(랭크/지역/MMR), 매너 평가
- [ ] 확정 매칭 → 캘린더 자동 반영
