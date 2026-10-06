it 'does something' do
  @@n = do_something
  expect(@@n[:foo]).wont_be_within_delta 42
end
