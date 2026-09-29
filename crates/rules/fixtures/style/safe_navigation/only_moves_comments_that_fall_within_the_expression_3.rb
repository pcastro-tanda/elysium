# comment one
def foobar
  if FOO::BAR
  ^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
    # comment 2
    FOO::BAR.bar
  end
end
