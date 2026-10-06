it 'does something' do
  @@n = do_something
  expect(@@n).must_include 42
end
