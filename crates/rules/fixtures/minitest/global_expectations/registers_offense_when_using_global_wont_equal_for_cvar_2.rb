it 'does something' do
  @@n = do_something
  @@n.wont_equal 42
  ^^^ Use `expect(@@n)` instead.
end
