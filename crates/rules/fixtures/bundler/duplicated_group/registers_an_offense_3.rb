group :test, foo: true, bar: true do
  gem 'activesupport'
end

group :test, bar: true, foo: true do
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Gem group `:test, bar: true, foo: true` already defined on line 1 of the Gemfile.
  gem 'rspec'
end
