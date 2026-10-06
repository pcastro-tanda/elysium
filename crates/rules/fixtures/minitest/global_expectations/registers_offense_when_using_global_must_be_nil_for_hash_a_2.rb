it 'does something' do
  n = do_something
  n[:foo].must_be_nil 42
  ^^^^^^^ Use `expect(n[:foo])` instead.
end
