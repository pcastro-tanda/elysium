it 'does something' do
  @@n = do_something
  @@n.must_be_same_as 42
  ^^^ Use `value(@@n)` instead.
end
