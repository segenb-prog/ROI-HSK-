Pod::Spec.new do |spec|
  spec.name         = "HSKSDK"
  spec.version      = "1.0.0"
  spec.summary      = "HSK Platform SDK for iOS"
  spec.description  = <<-DESC
    The HSK SDK provides secure identity management, consent handling,
    and cryptographic verification for the HSK Platform.
  DESC
  spec.homepage     = "https://github.com/hskernel/ios-sdk"
  spec.license      = { :type => "MIT", :file => "LICENSE" }
  spec.author       = { "HSK Platform" => "sdk@hskernel.io" }
  spec.platform     = :ios, "14.0"
  spec.swift_version = "5.9"
  spec.source       = { :git => "https://github.com/hskernel/ios-sdk.git", :tag => "#{spec.version}" }
  spec.source_files = "Sources/**/*.swift"
  spec.frameworks   = "Foundation", "CryptoKit", "LocalAuthentication", "AuthenticationServices"
  spec.dependency "Alamofire", "~> 5.8"
  spec.dependency "KeychainAccess", "~> 4.2"
end
