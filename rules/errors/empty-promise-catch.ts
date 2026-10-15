// Test cases for sloptopop.errors.empty-promise-catch (semgrep --test format).

function fireAndForget(id: string) {
  // ruleid: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch(() => {});

  // ruleid: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch((e) => {});

  // ruleid: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch(() => undefined);

  // ruleid: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch(function () {});

  // ruleid: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch((err: unknown) => {
    // best effort
  });

  // ruleid: sloptopop.errors.empty-promise-catch
  void refresh().then(render).catch(() => {});
}

function handled(id: string) {
  // ok: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch((e) => logger.warn("analytics failed", e));

  // ok: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch((e) => {
    console.error(e);
  });

  // ok: sloptopop.errors.empty-promise-catch
  sendAnalytics(id).catch(reportError);
}
