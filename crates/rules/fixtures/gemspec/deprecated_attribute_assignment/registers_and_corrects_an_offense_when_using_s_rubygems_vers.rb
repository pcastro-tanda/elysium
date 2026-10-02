Gem::Specification.new do |s|
  s.name = 'your_cool_gem_name'
  s.date = Time.now.strftime('%Y-%m-%d')
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not set `date` in gemspec.
  s.bindir = 'exe'
end
