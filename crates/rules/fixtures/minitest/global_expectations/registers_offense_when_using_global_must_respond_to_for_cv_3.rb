it 'does something' do
  @@n = do_something
  @@n.must_respond_to 42
  ^^^ Use `_(@@n)` instead.
end
