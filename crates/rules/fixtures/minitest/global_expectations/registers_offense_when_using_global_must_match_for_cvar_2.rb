it 'does something' do
  @@n = do_something
  @@n.must_match 42
  ^^^ Use `expect(@@n)` instead.
end
