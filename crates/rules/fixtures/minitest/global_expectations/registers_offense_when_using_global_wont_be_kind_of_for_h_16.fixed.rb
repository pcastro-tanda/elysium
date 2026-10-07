it 'does something' do
  $n = do_something
  value($n[:foo]).wont_be_kind_of 42
end
