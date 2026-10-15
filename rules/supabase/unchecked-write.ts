// Test cases for sloptopop.supabase.unchecked-write (semgrep --test format).

export async function createOrder(supabase: SupabaseClient, order: Order) {
  // ruleid: sloptopop.supabase.unchecked-write
  await supabase.from("orders").insert(order);

  // ruleid: sloptopop.supabase.unchecked-write
  await supabase.from("orders").update({ status: "paid" }).eq("id", order.id);

  // ruleid: sloptopop.supabase.unchecked-write
  await supabase
    .from("orders")
    .upsert(order, { onConflict: "id" });

  // ruleid: sloptopop.supabase.unchecked-write
  const { data } = await supabase.from("orders").insert(order).select().single();

  // ruleid: sloptopop.supabase.unchecked-write
  await ctx.supabaseAdmin.from("orders").delete().eq("id", order.id);

  return data;
}

export async function checked(supabase: SupabaseClient, order: Order) {
  // ok: sloptopop.supabase.unchecked-write
  const { error } = await supabase.from("orders").insert(order);
  if (error) throw error;

  // ok: sloptopop.supabase.unchecked-write
  const { data, error: updateError } = await supabase.from("orders").update(order).eq("id", order.id);

  // ok: sloptopop.supabase.unchecked-write
  await supabase.from("orders").insert(order).throwOnError();

  // ok: sloptopop.supabase.unchecked-write
  await supabase.from("orders").insert(order).then(({ error }) => report(error));

  // ok: sloptopop.supabase.unchecked-write
  const result = await supabase.from("orders").delete().eq("id", order.id);

  // ok: sloptopop.supabase.unchecked-write
  await supabase.from("orders").select("*");

  // ok: sloptopop.supabase.unchecked-write
  await supabase.storage.from("avatars").update(path, file);

  // ok: sloptopop.supabase.unchecked-write
  await audit(supabase.from("orders").insert(order));

  return [data, updateError, result];
}
