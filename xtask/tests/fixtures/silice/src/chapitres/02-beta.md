# Chapter 2 - Beta (synthetic fixture)

## 2. Maps

### MAP_BETA_CITY

## 3. Scenes

```text
[Beta_Sc01_Yes] GUIDE
You said yes.

[Beta_Sc01_No] GUIDE
You said no.

[Beta_Sc01_Hello] {PLAYER}
Hello, {PLAYER}!
```

```text
msgbox(Beta_Sc01_Hello)
msgbox(Beta_Sc01_Yes | No)
if flag(FLAG_SIL_ALPHA_DONE):
    warp(MAP_ALPHA_TOWN)
```

| Event | Text |
|---|---|
| Greeting | Beta_Sc01_Hello |
