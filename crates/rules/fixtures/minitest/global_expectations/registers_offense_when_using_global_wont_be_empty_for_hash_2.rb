it 'does something' do
  n = do_something
  n[:foo].wont_be_empty 42
  ^^^^^^^ Use `expect(n[:foo])` instead.
end
