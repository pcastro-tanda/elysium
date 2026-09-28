str = 'foo'
str.define_singleton_method :bar do |baz|
  return unless baz
  replace baz
end
