it 'does something' do
  @@n = do_something
  expect(@@n[:foo]).must_be_within_delta 42
end
