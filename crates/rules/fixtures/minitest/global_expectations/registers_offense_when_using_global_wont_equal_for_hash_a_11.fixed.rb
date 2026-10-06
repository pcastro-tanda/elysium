it 'does something' do
  $n = do_something
  expect($n[:foo]).wont_equal 42
end
