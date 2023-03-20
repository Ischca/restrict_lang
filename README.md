# vsc-dev-user-rust-template 
Visual Studio Codespace for Rust Development on Ubuntu (as non-root user)

![Create Codespace](/README_image_01.png)

Visit https://online.visualstudio.com/environments and create a new codespace as shown above.  Read about Microsoft container templates [here](https://github.com/microsoft/vscode-dev-containers).

## Contents
* Ubuntu 20.04 LTS
* git
* gcc
* python
* rust 

```
C:\github\vsc-dev-user-rust-template\.devcontainer>docker build --tag test .
...
...
...
C:\github\vsc-dev-user-rust-template\.devcontainer>docker images test
REPOSITORY          TAG                 IMAGE ID            CREATED              SIZE
test                latest                                  About a minute ago   1.19GB
```

## Customization

You can lookup valid linux timezones [here](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones).  Ubuntu country mirrors are described [here](https://wiki.ubuntu.com/Mirrors#Country_mirror_requirements).

### Example: Default
```
ARG REPO="archive"
ENV TZ="America/New_York"
```

### Example: Hong Kong
```
ARG REPO="hk.archive"
ENV TZ="Asia/Hong_Kong"
```

<br />
<br />

# Launch Codespaces

![Using Codespace](/README_image_02.png)

<br />
<br />

# How to use template

### Use this template
Press `Use this template` to create a new repository from the template.

![Using Template](/README_image_03.png)

---

<br />

### Create a new repository

The `new repository name` becomes the directory name and therefore also the default rust package name.

![Using Template](/README_image_04.png)

---

<br />

### Cargo Init 

Initialize using `cargo init --vcs git` [[cargo init]](https://doc.rust-lang.org/cargo/commands/cargo-init.html)

![Using Template](/README_image_05.png)

<br />



# TODO

https://code.visualstudio.com/docs/editor/integrated-terminal (shellarg -l)
