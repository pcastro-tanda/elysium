it 'does something' do
  @@n = do_something
  @@n[:foo].wont_respond_to 42
  ^^^^^^^^^ Use `expect(@@n[:foo])` instead.
end
