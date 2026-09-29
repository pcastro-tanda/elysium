h2[k2] = Hash.new { |h3,k3|
                  ^ Avoid using `{...}` for multi-line blocks.
  h3[k3] = 0
}

x = done? list.reject { |e|
  e.nil?
}
