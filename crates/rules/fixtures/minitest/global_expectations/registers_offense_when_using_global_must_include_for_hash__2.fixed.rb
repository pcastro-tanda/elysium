it 'does something' do
  n = do_something
  expect(n[:foo]).must_include 42
end
