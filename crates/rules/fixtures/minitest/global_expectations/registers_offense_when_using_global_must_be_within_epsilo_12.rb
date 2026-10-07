it 'does something' do
  @@n = do_something
  @@n.must_be_within_epsilon 42
  ^^^ Use `_(@@n)` instead.
end
