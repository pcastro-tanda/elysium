it 'does something' do
  @@n = do_something
  expect(@@n[:foo]).wont_match 42
end
