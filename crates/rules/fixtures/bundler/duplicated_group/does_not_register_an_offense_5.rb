platforms :ruby do
  group :default do
    gem 'openssl'
  end
end

platforms :jruby do
  group :default do
    gem 'jruby-openssl'
  end
end
