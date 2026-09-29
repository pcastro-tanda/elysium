# comment one
def foobar
  if $FOO
  ^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
    # comment 2
    $FOO.bar
  end
end
