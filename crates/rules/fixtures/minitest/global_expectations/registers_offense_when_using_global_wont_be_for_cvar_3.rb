it 'does something' do
  @@n = do_something
  @@n.wont_be 42
  ^^^ Use `value(@@n)` instead.
end
