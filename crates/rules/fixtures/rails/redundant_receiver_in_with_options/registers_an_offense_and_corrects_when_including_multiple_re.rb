with_options options: false do |merger|
  merger.invoke(merger.something)
                ^^^^^^ Redundant receiver in `with_options`.
  ^^^^^^ Redundant receiver in `with_options`.
end
