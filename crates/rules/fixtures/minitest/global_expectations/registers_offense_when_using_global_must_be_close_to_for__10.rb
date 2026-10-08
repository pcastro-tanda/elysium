it 'does something' do
  @@n = do_something
  @@n[:foo].must_be_close_to 42
  ^^^^^^^^^ Use `expect(@@n[:foo])` instead.
end
