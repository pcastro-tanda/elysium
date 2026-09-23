render :json => {:a => messages,
                 :b => :json}, :status => 404
def example
  a(
    b: :c,
    d: e(
      f: g
    ), h: :i)
end
