Gem::Specification.new do |s|
  s.name = 'your_cool_gem_name'
  s.test_files = Dir.glob('test/**/*')
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not set `test_files` in gemspec.
  s.bindir = 'exe'
end
