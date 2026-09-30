def payload
  {
    type: 'action',
    params: {
      page: get_page_name,
      email: @action.member.email,
      mailing_id: @mailing_id
    }.reverse_merge(@action.form_data)
      .merge(UserLanguageISO.for(page.language))
      ^^^^^^ Use 2 (not 0) spaces for indenting an expression spanning multiple lines.
      .tap do |params|
      ^^^^ Use 2 (not 0) spaces for indenting an expression spanning multiple lines.
        params[:country] = country(member.country) if member.country.present?
        params[:action_bucket] = data[:bucket] if data.key? :bucket
      end
  }.deep_symbolize_keys
end
