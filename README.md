# EC Wallet

An EVM Chain Cryptocurrency Wallet for Android.



## Warning

### This project is for entertainment/educational purposes only. DO NOT use it in a production environment.

### The user assumes all responsibility and consequences arising from the use of this software.

### The developer does not guarantee the cryptographic security of the encryption algorithms used.



## Build

**Note:** The following steps use Debian as an example. The process for Windows or other Linux distributions is similar.

1. Install Rust (>=1.88.0) and configure the Android toolchain:

   `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

   `rustup toolchain add aarch64-linux-android`

2. Install `dioxus-cli`:

   ```
   cargo install dioxus-cli
   ```

3. Install the JDK:

   `sudo apt install openjdk-11-jdk`

4. Install the Android SDK:

   (1) Download [commandlinetools](https://developer.android.com/studio#command-tools)(Download "commandlinetools-xxxx-latest.zip" and unzip it to /usr/local)

   (2) Download Android SDK

   ```
   cd /usr/local
   mkdir -p android-sdk/cmdline-tools
   mv cmdline-tools android-sdk/cmdline-tools/latest 2>/dev/null || true
   sudo android-sdk/cmdline-tools/latest/bin/sdkmanager --sdk_root=/usr/local/android-sdk "build-tools;36.0.0" "platforms;android-36"
   ```

5. Install the Android NDK:

   [android-ndk](https://dl.google.com/android/repository/android-ndk-r29-linux.zip)(Download and unzip it to /usr/local)

6. Configure environment variables:

   Set `ANDROID_HOME` to point to the `android-sdk` and `ANDROID_NDK_HOME` to point to `android-ndk-r29`.

7. (Optional) Prepare a signing private key:

   (1) Create an `ec.conf` file (replace `<hex_d>` with your private key in hex format):

   ```
   asn1=SEQUENCE:ec_privatekey
   
   [ec_privatekey]
   version=INTEGER:1
   privateKey=FORMAT:HEX,OCTETSTRING:<hex_d>
   parameters=EXPLICIT:0,OBJECT:prime256v1
   ```

   (2) Generate the DER file, then convert to PEM:

   ```
   openssl asn1parse -genconf ec.conf -out ec.der -noout
   openssl ec -inform DER -in ec.der -out ec.pem
   chmod 600 ec.pem
   ```

   (3) Generate a self-signed certificate based on the private key:

   `openssl req -new -x509 -key ec.pem -out cert.pem -days 10000`

   Follow the prompts to fill in information (Country, Org, etc.), or leave them blank.

   (4) Package as a PKCS12 JKS:

   ```
   # Note: You will be asked to set a password; please remember it (e.g., 123456).
   # The name after -name is the jks_alias you will write in Dioxus.toml.
   openssl pkcs12 -export -in cert.pem -inkey ec.pem -out key.jks -name key
   ```

   (5) Add the following information to `Dioxus.toml`:

   ```
   [bundle.android]
   jks_file = "key.jks"
   jks_password = "<your password>"
   key_alias = "key"
   key_password = "<your password>"
   ```

8. Compile and Package:

   `dx build --release --target aarch64-linux-android --android`

The packaged APK will be located at:

- `target/dx/ECWallet/release/android/app/app/build/outputs/apk/debug/app-debug.apk` (Unsigned)
- `target/dx/ECWallet/release/android/app/app/build/outputs/apk/release/app-release.apk` (Signed)

**Note:** If the build fails, ensure the NDK path matches the configuration in `.cargo/config.toml`.