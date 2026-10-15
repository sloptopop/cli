// Test cases for sloptopop.errors.empty-catch (semgrep --test format).

async function saveOrder(order: Order) {
  // ruleid: sloptopop.errors.empty-catch
  try {
    await db.save(order);
  } catch (e) {}
}

async function saveWithComment(order: Order) {
  // ruleid: sloptopop.errors.empty-catch
  try {
    await db.save(order);
  } catch (error) {
    // ignore: the order is retried later
  }
}

function parseOrDefault(text: string) {
  // ruleid: sloptopop.errors.empty-catch
  try {
    return JSON.parse(text);
  } catch {}
}

function typedBinding() {
  // ruleid: sloptopop.errors.empty-catch
  try {
    run();
  } catch (e: unknown) {
    /* nothing to do */
  }
}

function withFinally() {
  // ruleid: sloptopop.errors.empty-catch
  try {
    run();
  } catch (e) {
  } finally {
    cleanup();
  }
}

async function logged(order: Order) {
  // ok: sloptopop.errors.empty-catch
  try {
    await db.save(order);
  } catch (e) {
    logger.error("saveOrder: save failed", e);
    throw e;
  }
}

function rethrown() {
  // ok: sloptopop.errors.empty-catch
  try {
    run();
  } catch (e) {
    throw e;
  }
}

function onlyFinally() {
  // ok: sloptopop.errors.empty-catch
  try {
    run();
  } finally {
    cleanup();
  }
}
