do_something do
  OpenSSL::Digest::SHA1.new
  ^^^^^^^^^^^^^^^^^^^^^^^^^ Use `OpenSSL::Digest.new('SHA1')` instead of `OpenSSL::Digest::SHA1.new`.
end
