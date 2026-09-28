group :test, foo: true, bar: true do
  gem 'activesupport'
end

group :test, foo: true, bar: true do
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Gem group `:test, foo: true, bar: true` already defined on line 1 of the Gemfile.
  gem 'rspec'
end
