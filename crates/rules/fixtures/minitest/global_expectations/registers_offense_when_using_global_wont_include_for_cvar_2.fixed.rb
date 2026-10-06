it 'does something' do
  @@n = do_something
  expect(@@n).wont_include 42
end
