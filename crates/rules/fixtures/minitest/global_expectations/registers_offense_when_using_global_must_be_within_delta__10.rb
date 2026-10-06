it 'does something' do
  @@n = do_something
  @@n.must_be_within_delta 42
  ^^^ Use `expect(@@n)` instead.
end
